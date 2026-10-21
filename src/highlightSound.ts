// A short tick when the pointer moves onto a wheel wedge.
// Tangerine's landing page plays /sounds/highlight.wav (60 ms, quiet)
// on each new highlight. This is an original tick of the same length,
// not a copy of that file.

let context: AudioContext | null = null;
let tick: AudioBuffer | null = null;
let playing: AudioBufferSourceNode | null = null;

function audioContext(): AudioContext | null {
  const Ctx = window.AudioContext;
  if (!Ctx) return null;
  context ??= new Ctx({ latencyHint: "interactive" });
  return context;
}

function tickBuffer(ctx: AudioContext): AudioBuffer {
  if (tick && tick.sampleRate === ctx.sampleRate) return tick;
  const seconds = 0.06;
  const length = Math.floor(ctx.sampleRate * seconds);
  const buffer = ctx.createBuffer(1, length, ctx.sampleRate);
  const data = buffer.getChannelData(0);
  for (let i = 0; i < length; i += 1) {
    const t = i / ctx.sampleRate;
    // Fast decay, mostly the first 15 ms, same shape as a UI highlight click.
    const envelope = Math.exp(-t * 85);
    data[i] = (Math.random() * 2 - 1) * envelope;
  }
  tick = buffer;
  return buffer;
}

function startTick(ctx: AudioContext): void {
  try {
    playing?.stop();
  } catch {
    // The previous tick already finished.
  }
  playing = null;

  const source = ctx.createBufferSource();
  source.buffer = tickBuffer(ctx);
  const filter = ctx.createBiquadFilter();
  filter.type = "highpass";
  filter.frequency.value = 1800;
  const gain = ctx.createGain();
  gain.gain.value = 0.22;
  source.connect(filter);
  filter.connect(gain);
  gain.connect(ctx.destination);
  source.onended = () => {
    if (playing === source) playing = null;
  };
  playing = source;
  source.start();
}

/** Play when the highlighted wedge changes. No sound when the pointer leaves the wheel. */
export function playHighlight(): void {
  const ctx = audioContext();
  if (!ctx) return;
  if (ctx.state === "suspended") {
    void ctx.resume().then(() => {
      if (ctx.state === "running") startTick(ctx);
    });
    return;
  }
  if (ctx.state === "running") startTick(ctx);
}
