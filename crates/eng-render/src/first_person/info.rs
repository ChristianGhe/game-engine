//! What the first-person `info` target holds for each pixel (ADR-0020): the
//! region it shows, the class of surface, and the object's id, packed into
//! the two `u32`s of an `Rg32Uint` texel.

/// The kind of surface a pixel shows. Its [`SurfaceClass::code`] is what
/// the shaders read, so the codes never change; a new class takes a new
/// code. The walls are split by facing so that the light pass's fake
/// contrast (Doom's wall-direction term) needs no normals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SurfaceClass {
    /// Nothing was drawn there: the clear value, where the sky shows. No
    /// vertex may have it.
    None,
    /// A surface facing up.
    Floor,
    /// A surface facing down.
    Ceiling,
    /// A vertical surface at an angle to both axes.
    Wall,
    /// A vertical surface facing +X or -X (it runs along Y).
    WallFacingX,
    /// A vertical surface facing +Y or -Y (it runs along X).
    WallFacingY,
    /// A thing's sprite (rung 2, T11).
    Sprite,
}

impl SurfaceClass {
    /// Every class, in code order.
    pub const ALL: [Self; 7] = [
        Self::None,
        Self::Floor,
        Self::Ceiling,
        Self::Wall,
        Self::WallFacingX,
        Self::WallFacingY,
        Self::Sprite,
    ];

    /// Its code in `info`: 0 for [`SurfaceClass::None`], then 1 to 6 in the
    /// order declared.
    pub const fn code(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Floor => 1,
            Self::Ceiling => 2,
            Self::Wall => 3,
            Self::WallFacingX => 4,
            Self::WallFacingY => 5,
            Self::Sprite => 6,
        }
    }

    /// The class of `code`, if it is one.
    pub const fn from_code(code: u8) -> Option<Self> {
        if (code as usize) < Self::ALL.len() {
            Some(Self::ALL[code as usize])
        } else {
            None
        }
    }
}

/// One pixel of the `info` target, or one vertex's share of it: what the
/// passes after `opaque` know of the surface there.
///
/// **Packing**, into `Rg32Uint`'s two words:
/// - word 0, red: the region in bits 0 to 15, the class's code in bits 16
///   to 23, and bits 24 to 31 zero (kept for later flags);
/// - word 1, green: the object id, all 32 bits.
///
/// So `region = r & 0xffff`, `class = (r >> 16) & 0xff` and `id = g` in
/// WGSL. A pixel nothing was drawn into keeps `opaque`'s clear value,
/// (0, 0), which is [`Info::NONE`]: class [`SurfaceClass::None`].
/// The region sits beside the class because light and fog read both, and
/// the id gets a word of its own because it names a world surface or, for
/// a sprite, a thing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Info {
    /// The region whose light and fog the pixel takes: an index into the
    /// region looks the light and fog passes read (rung 2, T10).
    pub region: u16,
    pub class: SurfaceClass,
    /// The object the pixel shows, as the extraction numbers them: a
    /// world surface, or a thing for a sprite. Plan addition A1 counts its
    /// pixels.
    pub id: u32,
}

impl Info {
    /// The clear value: nothing drawn.
    pub const NONE: Self = Self {
        region: 0,
        class: SurfaceClass::None,
        id: 0,
    };

    /// The bits of word 0 that hold nothing yet.
    pub const RESERVED: u32 = 0xff00_0000;

    /// Its two words, as `info` stores them.
    pub const fn pack(self) -> [u32; 2] {
        [
            self.region as u32 | (self.class.code() as u32) << 16,
            self.id,
        ]
    }

    /// What two words of `info` say, or `None` if a reserved bit is set or
    /// the class's code is not one.
    pub const fn unpack(words: [u32; 2]) -> Option<Self> {
        if words[0] & Self::RESERVED != 0 {
            return None;
        }
        let Some(class) = SurfaceClass::from_code((words[0] >> 16) as u8) else {
            return None;
        };
        Some(Self {
            region: (words[0] & 0xffff) as u16,
            class,
            id: words[1],
        })
    }
}
