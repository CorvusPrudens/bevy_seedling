use std::sync::Arc;

use firewheel::collector::ArcGc;

/// HRIR subjects embedded directly in the binary.
///
/// The embedded subjects were chosen at complete random, expect for 1040.
/// There are many more available from the
/// [IRCAM database](http://recherche.ircam.fr/equipes/salles/listen/download.html).
/// The processed data can be found
/// [in this crate's repository](https://github.com/corvusprudens/firewheel-ircam-hrtf).
///
/// The data is collected from short "impulses" played from all angles
/// and recorded at the ear canal. The resulting recordings capture
/// how sounds are affected by the subject's torso, head, and ears.
///
/// The effect can be rather personal as a result, but the
/// `IRC_1040` subject is commonly cited as a good default.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "bevy_reflect", derive(bevy_reflect::Reflect))]
pub enum Subject {
    /// Male, short hair.
    #[cfg(feature = "irc1005")]
    Irc1005,
    /// Male, bulky hairstyle.
    #[cfg(feature = "irc1012")]
    Irc1012,
    /// Female, short hair.
    #[cfg(feature = "irc1015")]
    Irc1015,
    /// Male, medium hair.
    #[cfg(feature = "irc1018")]
    Irc1018,
    /// Female, long hair.
    #[cfg(feature = "irc1028")]
    Irc1028,
    /// Female, long hair.
    #[cfg(feature = "irc1029")]
    Irc1029,
    /// Male, long and bulky hairstyle.
    #[cfg(feature = "irc1034")]
    Irc1034,
    /// Male, short hair.
    ///
    /// Commonly cited as a good default subject.
    #[default]
    Irc1040,
    /// Male, bulky hairstyle.
    #[cfg(feature = "irc1042")]
    Irc1042,
    /// Female, curvy hairstyle.
    #[cfg(feature = "irc1052")]
    Irc1052,
    /// Female, short hair.
    #[cfg(feature = "irc1053")]
    Irc1053,
}

impl AsRef<[u8]> for Subject {
    fn as_ref(&self) -> &[u8] {
        match self {
            #[cfg(feature = "irc1005")]
            Subject::Irc1005 => include_bytes!("../assets/irc_1005_c.bin"),
            #[cfg(feature = "irc1012")]
            Subject::Irc1012 => include_bytes!("../assets/irc_1012_c.bin"),
            #[cfg(feature = "irc1015")]
            Subject::Irc1015 => include_bytes!("../assets/irc_1015_c.bin"),
            #[cfg(feature = "irc1018")]
            Subject::Irc1018 => include_bytes!("../assets/irc_1018_c.bin"),
            #[cfg(feature = "irc1028")]
            Subject::Irc1028 => include_bytes!("../assets/irc_1028_c.bin"),
            #[cfg(feature = "irc1029")]
            Subject::Irc1029 => include_bytes!("../assets/irc_1029_c.bin"),
            #[cfg(feature = "irc1034")]
            Subject::Irc1034 => include_bytes!("../assets/irc_1034_c.bin"),
            Subject::Irc1040 => include_bytes!("../assets/irc_1040_c.bin"),
            #[cfg(feature = "irc1042")]
            Subject::Irc1042 => include_bytes!("../assets/irc_1042_c.bin"),
            #[cfg(feature = "irc1052")]
            Subject::Irc1052 => include_bytes!("../assets/irc_1052_c.bin"),
            #[cfg(feature = "irc1053")]
            Subject::Irc1053 => include_bytes!("../assets/irc_1053_c.bin"),
        }
    }
}

/// A cheaply cloneable collection of HRIR bytes.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "bevy_reflect", derive(bevy_reflect::Reflect))]
#[cfg_attr(feature = "bevy_reflect", reflect(opaque))]
pub struct SubjectBytes(ArcGc<[u8]>);

impl SubjectBytes {
    /// Copy `bytes` into a reference-counted [`SubjectBytes`].
    pub fn new(bytes: &[u8]) -> Self {
        Self(ArcGc::new_unsized(|| Arc::from(bytes)))
    }
}

impl AsRef<[u8]> for SubjectBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
