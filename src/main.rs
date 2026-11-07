use std::{
    collections::VecDeque,
    fs::File,
    io::{BufWriter, Write, stdin, stdout},
    ptr::null_mut,
    sync::mpsc,
    thread,
};

use windows_sys::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

const CHUNK_SIZE: usize = 4096;

fn main() {
    let pid = {
        let mut input = String::new();
        print!("pid: ");
        stdout().flush().unwrap();
        stdin().read_line(&mut input).unwrap();
        input.trim().parse().unwrap()
    };

    unsafe { CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED as u32) };
    let mut client = wasapi::AudioClient::new_application_loopback_client(pid, true).unwrap();

    let wavefmt = wasapi::WaveFormat::new(32, 32, wasapi::SampleType::Float, 44_100, 2, None);
    client
        .initialize_client(
            &wavefmt,
            wasapi::Direction::Capture,
            wasapi::StreamMode::EventsShared {
                autoconvert: true,
                buffer_duration_hns: 10,
            },
            None,
        )
        .unwrap();

    let handle = client.set_get_eventhandle().unwrap();
    let capture = client.get_audiocaptureclient().unwrap();

    client.start_stream().unwrap();

    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let writer = thread::spawn(move || {
        let path = format!("{pid}.pcm");
        let file = File::create(&path).unwrap();
        let mut file = BufWriter::new(file);
        println!("writing to {path}");
        for chunk in &rx {
            file.write_all(&chunk).unwrap();
        }
        println!("writer finished");
    });

    let blockalign = wavefmt.get_blockalign() as usize;
    let chunk_bytes = blockalign * CHUNK_SIZE;
    let mut samples = VecDeque::new();
    println!("recording");
    loop {
        while samples.len() > chunk_bytes {
            let chunk: Vec<u8> = samples.drain(..chunk_bytes).collect();
            tx.send(chunk).unwrap();
        }

        if let Some(new_frames) = capture.get_next_packet_size().unwrap()
            && new_frames > 0
        {
            capture.read_from_device_to_deque(&mut samples).unwrap();
        }

        if handle.wait_for_event(3000).is_err() {
            println!("capture stream timed out");
            client.stop_stream().unwrap();
            break;
        }
    }

    println!("waiting for writer");
    writer.join().unwrap();
    println!("done");
}
