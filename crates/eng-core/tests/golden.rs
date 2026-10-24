//! Golden layout of `PlayerCommand`, the one input type (spec 6.4).
//! PROTECTED ORACLE: recordings and the network carry these bytes.

use eng_core::{PlayerCommand, Tick};
use eng_schema_core::{from_bytes, to_bytes};

#[test]
fn player_command_layout_is_frozen() {
    let cmd = PlayerCommand {
        tick: Tick(0x0102),
        buttons: 0x8000_0001,
        forward: 127,
        side: -128,
        view_yaw: 0x4000,
        view_pitch: 0xfff0,
        select: 3,
    };
    let bytes = to_bytes(&cmd);
    let expected: Vec<u8> = [
        &0x0102u64.to_le_bytes()[..],
        &0x8000_0001u32.to_le_bytes(),
        &[127, 0x80],
        &0x4000u16.to_le_bytes(),
        &0xfff0u16.to_le_bytes(),
        &[3],
    ]
    .concat();
    assert_eq!(bytes, expected);
    assert_eq!(from_bytes::<PlayerCommand>(&bytes).unwrap(), cmd);
    assert_eq!(cmd.repeated_at(Tick(9)).tick, Tick(9));
}
