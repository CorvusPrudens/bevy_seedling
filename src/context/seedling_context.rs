use core::any::Any;
use core::error::Error;
use firewheel::{
    backend::{AudioBackend, DeviceInfo},
    channel_config::ChannelConfig,
    clock::{ClockSamples, ClockSeconds, MusicalTime, MusicalTransport},
    error::{AddEdgeError, UpdateError},
    event::{NodeEvent, NodeEventType},
    graph::{Edge, EdgeID, NodeEntry, PortIdx},
    node::{AudioNode, Constructor, DynAudioNode, NodeID},
    FirewheelCtx, StreamInfo,
};
use smallvec::SmallVec;

/// A type-erased Firewheel context.
///
/// This allows applications to treat all backends identically after construction.
pub struct SeedlingContext(Box<dyn SeedlingContextWrapper>);

impl core::fmt::Debug for SeedlingContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SeedlingContext").finish_non_exhaustive()
    }
}

impl core::ops::Deref for SeedlingContext {
    type Target = dyn SeedlingContextWrapper;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl core::ops::DerefMut for SeedlingContext {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut()
    }
}

impl SeedlingContext {
    /// Construct a new [`SeedlingContext`].
    pub fn new<B>(context: FirewheelCtx<B>) -> Self
    where
        B: AudioBackend + 'static,
        B::StreamError: Send + Sync + 'static,
    {
        Self(Box::new(context))
    }

    /// Add a new Firewheel node.
    pub fn add_node<T: AudioNode + 'static>(
        &mut self,
        node: T,
        configuration: Option<T::Configuration>,
    ) -> NodeID {
        self.add_node_dyn(ErasedNode::new(node, configuration))
    }

    /// Retrieve a node's state.
    ///
    /// If the given ID has no state or the expected type doesn't match,
    /// this returns `None`.
    pub fn node_state<T: 'static>(&self, node_id: NodeID) -> Option<&T> {
        self.node_state_dyn(node_id).and_then(|s| s.downcast_ref())
    }

    /// Retrieve a mutable reference to a node's state.
    ///
    /// If the given ID has no state or the expected type doesn't match,
    /// this returns `None`.
    pub fn node_state_mut<T: 'static>(&mut self, node_id: NodeID) -> Option<&mut T> {
        self.node_state_mut_dyn(node_id)
            .and_then(|s| s.downcast_mut())
    }
}

/// A dyn-compatible trait wrapper for a Firewheel context.
///
/// This allows applications to treat all backend identically
/// after construction.
pub trait SeedlingContextWrapper {
    /// Get a list of the available audio input devices.
    fn available_input_devices(&self) -> Vec<DeviceInfo>;

    /// Get a list of the available audio output devices.
    fn available_output_devices(&self) -> Vec<DeviceInfo>;

    /// Information about the running audio stream.
    ///
    /// Returns `None` if no audio stream is currently running.
    fn stream_info(&self) -> Option<&StreamInfo>;

    /// The current time of the clock in the number of seconds since the stream
    /// was started.
    ///
    /// Note, this clock is not perfectly accurate, but it is good enough for
    /// most use cases. This clock also correctly accounts for any output
    /// underflows that may occur.
    fn clock_now(&self) -> ClockSeconds;

    /// The current time of the sample clock in the number of samples (of a single
    /// channel of audio) that have been processed since the beginning of the
    /// stream.
    ///
    /// This is more accurate than the seconds clock, and is ideal for syncing
    /// events to a musical transport. Though note that this clock does not
    /// account for any output underflows that may occur.
    fn clock_samples(&self) -> ClockSamples;

    /// The current musical time of the transport.
    ///
    /// If no transport is currently active, then this will have a value of `0`.
    fn clock_musical(&self) -> MusicalTime;

    /// Set the musical transport to use.
    ///
    /// If an existing musical transport is already running, then the new
    /// transport will pick up where the old one left off. This allows you
    /// to, for example, change the tempo dynamically at runtime.
    ///
    /// If the message channel is full, then this will return an error.
    fn set_transport(
        &mut self,
        transport: Option<MusicalTransport>,
    ) -> Result<(), UpdateError<SeedlingContextError>>;

    /// Start or restart the musical transport.
    ///
    /// If the message channel is full, then this will return an error.
    fn start_or_restart_transport(&mut self) -> Result<(), UpdateError<SeedlingContextError>>;

    /// Pause the musical transport.
    ///
    /// If the message channel is full, then this will return an error.
    fn pause_transport(&mut self) -> Result<(), UpdateError<SeedlingContextError>>;

    /// Resume the musical transport.
    ///
    /// If the message channel is full, then this will return an error.
    fn resume_transport(&mut self) -> Result<(), UpdateError<SeedlingContextError>>;

    /// Stop the musical transport.
    ///
    /// If the message channel is full, then this will return an error.
    fn stop_transport(&mut self) -> Result<(), UpdateError<SeedlingContextError>>;

    /// Whether or not outputs are being hard clipped at 0dB.
    fn hard_clip_outputs(&self) -> bool;

    /// Set whether or not outputs should be hard clipped at 0dB to
    /// help protect the system's speakers.
    ///
    /// Note that most operating systems already hard clip the output,
    /// so this is usually not needed (TODO: Do research to see if this
    /// assumption is true.)
    ///
    /// If the message channel is full, then this will return an error.
    fn set_hard_clip_outputs(
        &mut self,
        hard_clip_outputs: bool,
    ) -> Result<(), UpdateError<SeedlingContextError>>;

    /// Update the firewheel context.
    ///
    /// This must be called regularly (i.e. once every frame).
    fn update(&mut self) -> Result<(), UpdateError<SeedlingContextError>>;

    /// The ID of the graph input node
    fn graph_in_node_id(&self) -> NodeID;

    /// The ID of the graph output node
    fn graph_out_node_id(&self) -> NodeID;

    /// Add a node to the audio graph.
    fn add_node_dyn(&mut self, node: ErasedNode) -> NodeID;

    /// Remove the given node from the audio graph.
    ///
    /// This will automatically remove all edges from the graph that
    /// were connected to this node.
    ///
    /// On success, this returns a list of all edges that were removed
    /// from the graph as a result of removing this node.
    ///
    /// This will return an error if a node with the given ID does not
    /// exist in the graph, or if the ID is of the graph input or graph
    /// output node.
    #[allow(clippy::result_unit_err)]
    fn remove_node(&mut self, node_id: NodeID) -> Result<SmallVec<[EdgeID; 4]>, ()>;

    /// Get information about a node in the graph.
    fn node_info(&self, id: NodeID) -> Option<&NodeEntry>;

    /// Get a type-erased, immutable reference to the custom state of a node.
    fn node_state_dyn(&self, id: NodeID) -> Option<&dyn Any>;

    /// Get a type-erased, mutable reference to the custom state of a node.
    fn node_state_mut_dyn(&mut self, id: NodeID) -> Option<&mut dyn Any>;

    /// Get a list of all the existing nodes in the graph.
    fn nodes(&self) -> Vec<&NodeEntry>;

    /// Get a list of all the existing edges in the graph.
    fn edges(&self) -> Vec<&Edge>;

    /// Set the number of input and output channels to and from the audio graph.
    ///
    /// Returns the list of edges that were removed.
    fn set_graph_channel_config(&mut self, channel_config: ChannelConfig) -> SmallVec<[EdgeID; 4]>;

    /// Add connections (edges) between two nodes to the graph.
    ///
    /// * `src_node` - The ID of the source node.
    /// * `dst_node` - The ID of the destination node.
    /// * `ports_src_dst` - The port indices for each connection to make,
    ///   where the first value in a tuple is the output port on `src_node`,
    ///   and the second value in that tuple is the input port on `dst_node`.
    /// * `check_for_cycles` - If `true`, then this will run a check to
    ///   see if adding these edges will create a cycle in the graph, and
    ///   return an error if it does. Note, checking for cycles can be quite
    ///   expensive, so avoid enabling this when calling this method many times
    ///   in a row.
    ///
    /// If successful, then this returns a list of edge IDs in order.
    ///
    /// If this returns an error, then the audio graph has not been
    /// modified.
    fn connect(
        &mut self,
        src_node: NodeID,
        dst_node: NodeID,
        ports_src_dst: &[(PortIdx, PortIdx)],
        check_for_cycles: bool,
    ) -> Result<SmallVec<[EdgeID; 4]>, AddEdgeError>;

    /// Remove connections (edges) between two nodes from the graph.
    ///
    /// * `src_node` - The ID of the source node.
    /// * `dst_node` - The ID of the destination node.
    /// * `ports_src_dst` - The port indices for each connection to make,
    ///   where the first value in a tuple is the output port on `src_node`,
    ///   and the second value in that tuple is the input port on `dst_node`.
    ///
    /// If none of the edges existed in the graph, then `false` will be
    /// returned.
    fn disconnect(
        &mut self,
        src_node: NodeID,
        dst_node: NodeID,
        ports_src_dst: &[(PortIdx, PortIdx)],
    ) -> bool;

    /// Remove all connections (edges) between two nodes in the graph.
    ///
    /// * `src_node` - The ID of the source node.
    /// * `dst_node` - The ID of the destination node.
    fn disconnect_all_between(
        &mut self,
        src_node: NodeID,
        dst_node: NodeID,
    ) -> SmallVec<[EdgeID; 4]>;

    /// Remove a connection (edge) via the edge's unique ID.
    ///
    /// If the edge did not exist in this graph, then `false` will be returned.
    fn disconnect_by_edge_id(&mut self, edge_id: EdgeID) -> bool;

    /// Get information about the given [Edge]
    fn edge(&self, edge_id: EdgeID) -> Option<&Edge>;

    /// Runs a check to see if a cycle exists in the audio graph.
    ///
    /// Note, this method is expensive.
    fn cycle_detected(&mut self) -> bool;

    /// Queue an event to be sent to an audio node's processor.
    ///
    /// Note, this event will not be sent until the event queue is flushed
    /// in [`FirewheelCtx::update`].
    fn queue_event(&mut self, event: NodeEvent);

    /// Queue an event to be sent to an audio node's processor.
    ///
    /// Note, this event will not be sent until the event queue is flushed
    /// in [`FirewheelCtx::update`].
    fn queue_event_for(&mut self, node_id: NodeID, event: NodeEventType);
}

impl<B: AudioBackend> SeedlingContextWrapper for FirewheelCtx<B>
where
    B::StreamError: core::error::Error + Send + Sync + 'static,
{
    fn available_input_devices(&self) -> Vec<DeviceInfo> {
        <FirewheelCtx<B>>::available_input_devices(self)
    }

    fn available_output_devices(&self) -> Vec<DeviceInfo> {
        <FirewheelCtx<B>>::available_output_devices(self)
    }

    fn stream_info(&self) -> Option<&StreamInfo> {
        <FirewheelCtx<B>>::stream_info(self)
    }

    fn clock_now(&self) -> ClockSeconds {
        <FirewheelCtx<B>>::clock_now(self)
    }

    fn clock_samples(&self) -> ClockSamples {
        <FirewheelCtx<B>>::clock_samples(self)
    }

    fn clock_musical(&self) -> MusicalTime {
        <FirewheelCtx<B>>::clock_musical(self)
    }

    fn set_transport(
        &mut self,
        transport: Option<MusicalTransport>,
    ) -> Result<(), UpdateError<SeedlingContextError>> {
        <FirewheelCtx<B>>::set_transport(self, transport).map_err(SeedlingContextError::map_update)
    }

    fn start_or_restart_transport(&mut self) -> Result<(), UpdateError<SeedlingContextError>> {
        <FirewheelCtx<B>>::start_or_restart_transport(self)
            .map_err(SeedlingContextError::map_update)
    }

    fn pause_transport(&mut self) -> Result<(), UpdateError<SeedlingContextError>> {
        <FirewheelCtx<B>>::pause_transport(self).map_err(SeedlingContextError::map_update)
    }

    fn resume_transport(&mut self) -> Result<(), UpdateError<SeedlingContextError>> {
        <FirewheelCtx<B>>::resume_transport(self).map_err(SeedlingContextError::map_update)
    }

    fn stop_transport(&mut self) -> Result<(), UpdateError<SeedlingContextError>> {
        <FirewheelCtx<B>>::stop_transport(self).map_err(SeedlingContextError::map_update)
    }

    fn hard_clip_outputs(&self) -> bool {
        <FirewheelCtx<B>>::hard_clip_outputs(self)
    }

    fn set_hard_clip_outputs(
        &mut self,
        hard_clip_outputs: bool,
    ) -> Result<(), UpdateError<SeedlingContextError>> {
        <FirewheelCtx<B>>::set_hard_clip_outputs(self, hard_clip_outputs)
            .map_err(SeedlingContextError::map_update)
    }

    fn update(&mut self) -> Result<(), UpdateError<SeedlingContextError>> {
        <FirewheelCtx<B>>::update(self).map_err(SeedlingContextError::map_update)
    }

    fn graph_in_node_id(&self) -> NodeID {
        <FirewheelCtx<B>>::graph_in_node_id(self)
    }

    fn graph_out_node_id(&self) -> NodeID {
        <FirewheelCtx<B>>::graph_out_node_id(self)
    }

    fn add_node_dyn(&mut self, node: ErasedNode) -> NodeID {
        <FirewheelCtx<B>>::add_dyn_node(self, node)
    }

    fn remove_node(&mut self, node_id: NodeID) -> Result<SmallVec<[EdgeID; 4]>, ()> {
        <FirewheelCtx<B>>::remove_node(self, node_id)
    }

    fn node_info(&self, id: NodeID) -> Option<&NodeEntry> {
        <FirewheelCtx<B>>::node_info(self, id)
    }

    fn node_state_dyn(&self, id: NodeID) -> Option<&dyn Any> {
        <FirewheelCtx<B>>::node_state_dyn(self, id)
    }

    fn node_state_mut_dyn(&mut self, id: NodeID) -> Option<&mut dyn Any> {
        <FirewheelCtx<B>>::node_state_dyn_mut(self, id)
    }

    fn nodes(&self) -> Vec<&NodeEntry> {
        <FirewheelCtx<B>>::nodes(self).collect()
    }

    fn edges(&self) -> Vec<&Edge> {
        <FirewheelCtx<B>>::edges(self).collect()
    }

    fn set_graph_channel_config(&mut self, channel_config: ChannelConfig) -> SmallVec<[EdgeID; 4]> {
        <FirewheelCtx<B>>::set_graph_channel_config(self, channel_config)
    }

    fn connect(
        &mut self,
        src_node: NodeID,
        dst_node: NodeID,
        ports_src_dst: &[(PortIdx, PortIdx)],
        check_for_cycles: bool,
    ) -> Result<SmallVec<[EdgeID; 4]>, AddEdgeError> {
        <FirewheelCtx<B>>::connect(self, src_node, dst_node, ports_src_dst, check_for_cycles)
    }

    fn disconnect(
        &mut self,
        src_node: NodeID,
        dst_node: NodeID,
        ports_src_dst: &[(PortIdx, PortIdx)],
    ) -> bool {
        <FirewheelCtx<B>>::disconnect(self, src_node, dst_node, ports_src_dst)
    }

    fn disconnect_all_between(
        &mut self,
        src_node: NodeID,
        dst_node: NodeID,
    ) -> SmallVec<[EdgeID; 4]> {
        <FirewheelCtx<B>>::disconnect_all_between(self, src_node, dst_node)
    }

    fn disconnect_by_edge_id(&mut self, edge_id: EdgeID) -> bool {
        <FirewheelCtx<B>>::disconnect_by_edge_id(self, edge_id)
    }

    fn edge(&self, edge_id: EdgeID) -> Option<&Edge> {
        <FirewheelCtx<B>>::edge(self, edge_id)
    }

    fn cycle_detected(&mut self) -> bool {
        <FirewheelCtx<B>>::cycle_detected(self)
    }

    fn queue_event(&mut self, event: NodeEvent) {
        <FirewheelCtx<B>>::queue_event(self, event)
    }

    fn queue_event_for(&mut self, node_id: NodeID, event: NodeEventType) {
        <FirewheelCtx<B>>::queue_event_for(self, node_id, event)
    }
}

/// A fully type-erased audio node.
pub struct ErasedNode(Box<dyn DynAudioNode>);

impl core::fmt::Debug for ErasedNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ErasedNode").finish_non_exhaustive()
    }
}

impl ErasedNode {
    /// Construct a new [`ErasedNode`].
    pub fn new<T: AudioNode + 'static>(node: T, configuration: Option<T::Configuration>) -> Self {
        Self(Box::new(Constructor::new(node, configuration)))
    }
}

impl DynAudioNode for ErasedNode {
    fn update(&mut self, cx: firewheel::node::UpdateContext) {
        self.0.update(cx)
    }

    fn construct_processor(
        &self,
        cx: firewheel::node::ConstructProcessorContext,
    ) -> Box<dyn firewheel::node::AudioNodeProcessor> {
        self.0.construct_processor(cx)
    }

    fn info(&self) -> firewheel::node::AudioNodeInfo {
        self.0.info()
    }
}

/// A type-erased context error.
#[derive(Debug)]
pub struct SeedlingContextError(Box<dyn Error + Send + Sync + 'static>);

impl core::fmt::Display for SeedlingContextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl SeedlingContextError {
    fn map_update<E: core::error::Error + Send + Sync + 'static>(
        error: UpdateError<E>,
    ) -> UpdateError<Self> {
        match error {
            UpdateError::GraphCompileError(e) => UpdateError::GraphCompileError(e),
            UpdateError::MsgChannelFull => UpdateError::MsgChannelFull,
            UpdateError::StreamStoppedUnexpectedly(e) => {
                UpdateError::StreamStoppedUnexpectedly(e.map(|e| Self(Box::new(e))))
            }
        }
    }
}

impl core::error::Error for SeedlingContextError {}
