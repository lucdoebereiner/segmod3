use clap::Clap;
use hound;
use std::cmp::max;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;

#[derive(Clap, Debug)]
#[clap(version = "1.0", author = "Luc Döbereiner <luc.doebereiner@gmail.com>")]
struct Opts {
    #[clap(short, long, default_value = "output.wav")]
    output_file: String,
    #[clap(short, long, default_value = "48000")]
    sample_rate: u32,
    #[clap(short, long)]
    frequencies: String,
    #[clap(short, long)]
    waveforms: String,
    #[clap(short, long)]
    phase_offsets: Option<String>,
    #[clap(short, long, default_value = "1")]
    breakpoints_per_cycle: u16,
}

#[derive(Debug, Clone, Copy)]
enum Wave {
    Sine,
    Cosine,
    Pulse,
    Triangle,
    SawUp,
    SawDown,
    DC(f64),
}

fn parse_wave(wave: &str) -> Wave {
    let lc_wave = wave.to_lowercase();
    match lc_wave.as_str() {
        "s" => Wave::Sine,
        "c" => Wave::Cosine,
        "p" => Wave::Pulse,
        "t" => Wave::Triangle,
        "u" => Wave::SawUp,
        "d" => Wave::SawDown,
        _ => {
            let dc = wave.parse::<f64>().unwrap();
            Wave::DC(dc)
        }
    }
}

fn wave(wave: Wave, cur_phase: f64, phase_offset: f64) -> f64 {
    match wave {
        Wave::Sine => sine(cur_phase, phase_offset),
        Wave::Cosine => cosine(cur_phase, phase_offset),
        Wave::Pulse => pulse(cur_phase, phase_offset),
        Wave::Triangle => triangle(cur_phase, phase_offset),
        Wave::SawUp => saw_up(cur_phase, phase_offset),
        Wave::SawDown => saw_down(cur_phase, phase_offset),
        Wave::DC(dc) => dc,
    }
}

fn load_waves_from_file(file_path: &str) -> Vec<Wave> {
    let file = File::open(file_path).expect("file wasn't found.");
    let reader = BufReader::new(file);

    let mut waves: Vec<Wave> = vec![];

    reader.lines().for_each(|line| {
        line.unwrap()
            .split_whitespace()
            .for_each(|w| waves.push(parse_wave(w)))
    });

    waves
}

fn load_floats_from_file(file_path: &str) -> Vec<f64> {
    let file = File::open(file_path).expect("file wasn't found.");
    let reader = BufReader::new(file);

    let mut numbers: Vec<f64> = vec![];

    reader.lines().for_each(|line| {
        line.unwrap()
            .split_whitespace()
            .for_each(|e| numbers.push(e.parse::<f64>().unwrap()))
    });

    numbers
}

fn freq_to_sample_length(freq: f64, sample_rate: u32) -> f64 {
    sample_rate as f64 / freq
}

fn freq_to_phase_inc(freq: f64, sample_rate: u32) -> f64 {
    freq / sample_rate as f64
}

fn lin_interp(x: f64, y1: f64, y2: f64) -> f64 {
    y1 + ((y2 - y1) * x)
}

fn sine(phase: f64, phase_offset: f64) -> f64 {
    ((phase + phase_offset) * (std::f64::consts::PI * 2.0)).sin()
}

fn cosine(phase: f64, phase_offset: f64) -> f64 {
    ((phase + phase_offset) * (std::f64::consts::PI * 2.0)).cos()
}

fn saw_up(phase: f64, phase_offset: f64) -> f64 {
    let ph = fmod(phase + phase_offset, 1.0);
    (ph * 2.0) - 1.0
}

fn saw_down(phase: f64, phase_offset: f64) -> f64 {
    let ph = fmod(phase + phase_offset, 1.0);
    ((ph * 2.0) - 1.0) * -1.0
}

fn fmod(numer: f64, denom: f64) -> f64 {
    let rquot = (numer / denom).floor();
    numer - rquot * denom
}

// fn total_length(freqs: &[f64], sample_rate: u32) -> u32 {
//     let mut sum = 0.0;
//     for f in freqs {
//         sum += freq_to_sample_length(*f, sample_rate);
//     }
//     sum.ceil() as u32
// }

fn triangle(phase: f64, phase_offset: f64) -> f64 {
    let ph = fmod(phase + phase_offset, 1.0);
    if ph <= 0.25 {
        lin_interp(ph / 0.25, 0.0, 1.0)
    } else if ph <= 0.75 {
        lin_interp((ph - 0.25) / 0.5, 1.0, -1.0)
    } else {
        lin_interp((ph - 0.75) / 0.25, -1.0, 0.0)
    }
}

fn pulse(phase: f64, phase_offset: f64) -> f64 {
    let ph = phase + phase_offset;
    if ph < 0.5 {
        1.0
    } else {
        -1.0
    }
}

fn synthesize(
    frequencies: &[f64],
    waves: &[Wave],
    breakpoints: u16,
    sample_rate: u32,
    phase_offsets: Option<&[f64]>,
) -> Vec<f64> {
    let ph_length = phase_offsets.map_or(0, |p| p.len());
    let n = max(ph_length, max(frequencies.len(), waves.len()));
    let mut output: Vec<f64> = vec![];
    let mut cur_wave = waves[0];
    let mut cur_phase_inc = freq_to_phase_inc(frequencies[0], sample_rate);
    let mut cur_phase = 0.0;
    let mut last_phase = 0.0;
    let mut i = 0;
    let mut phase_offset = phase_offsets.map_or(0.0, |p| p[i % p.len()]);

    while i < n {
        output.push(wave(cur_wave, cur_phase, phase_offset));
        cur_phase += cur_phase_inc;

        if (cur_phase >= 1.0) || ((breakpoints == 2) && (cur_phase >= 0.5) && (last_phase < 0.5)) {
            i += 1;
            cur_phase = fmod(cur_phase, 1.0);
            cur_phase_inc = freq_to_phase_inc(frequencies[i % frequencies.len()], sample_rate);
            cur_wave = waves[i % waves.len()];
            last_phase = cur_phase;
            phase_offset = phase_offsets.map_or(0.0, |p| p[i % p.len()]);
        }
    }

    output
}

fn write_sf(sample_rate: u32, output_file: String, audio: &[f64]) {
    let amplitude = 8_388_607 as f64;

    let wave_spec = hound::WavSpec {
        channels: 1,
        sample_rate: sample_rate,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(output_file, wave_spec).unwrap();

    for sample in audio.iter() {
        writer
            .write_sample((sample.min(1.0).max(-1.0) * amplitude) as i32)
            .unwrap();
    }
    writer.finalize().unwrap();
}

fn main() {
    let opts: Opts = Opts::parse();
    let sr = opts.sample_rate;
    let output_file = opts.output_file;
    let frequencies = load_floats_from_file(&opts.frequencies);
    let waves = load_waves_from_file(&opts.waveforms);
    let phase_offsets = opts.phase_offsets.map(|file| load_floats_from_file(&file));
    //        .as_deref();
    // let slice = match phase_offsets {
    //     None => None,
    //     Some(po) => Some(po.as_slice()),
    // };

    //    println!("{:?}", opts);

    let audio = synthesize(
        &frequencies,
        &waves,
        opts.breakpoints_per_cycle,
        opts.sample_rate,
        phase_offsets.as_deref(), //phase_offsets.map(|p| p.as_slice()),
    );

    write_sf(sr, output_file, &audio);
}
