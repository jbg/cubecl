//! Exercise the public transport and client together on a real Metal queue.
#![cfg(all(target_os = "macos", feature = "msl"))]

use cubecl_common::{device::Device, device_handle::DeviceHandle};
use cubecl_core::{self as cubecl, prelude::*};
use cubecl_environment::future::block_on;
use cubecl_runtime::runtime::Runtime;
use cubecl_wgpu::{WgpuDevice, WgpuRuntime};

#[cube(launch)]
fn fill(output: &mut [u32]) {
    if ABSOLUTE_POS < output.len() {
        output[ABSOLUTE_POS] = ABSOLUTE_POS as u32 + 17u32;
    }
}

#[test]
fn escaped_submission_failure_survives_native_completion_and_repeated_reads() {
    type R = WgpuRuntime;
    let device = WgpuDevice::default();
    let client = R::client(&device);
    let output = client.empty(64 * 4);
    fill::launch(
        &client,
        CubeCount::new_single(),
        CubeDim::new_1d(64),
        // SAFETY: the owned output contains exactly 64 initialized u32 slots.
        unsafe { BufferArg::from_raw_parts(output.clone(), 64) },
    );
    block_on(client.sync_buffers([&output])).unwrap();
    let bytes = client.read_one(output.clone()).unwrap();
    let actual = u32::from_ne_bytes(bytes[0..4].try_into().unwrap());
    assert_eq!(actual, 17);

    let raw = DeviceHandle::<<R as Runtime>::Server>::new(device.to_id());
    raw.submit(|_| panic!("unattributed native submission failure"));
    let first = client.flush().unwrap_err().to_string();
    assert!(first.contains("unattributed native submission failure"));
    for _ in 0..2 {
        let error = block_on(client.sync_buffers([&output]))
            .unwrap_err()
            .to_string();
        assert!(error.contains("unattributed native submission failure"));
        assert!(client.read_one(output.clone()).is_err());
    }
}
