// A soft click when the pointer moves onto a wheel wedge.
// Short sine, quiet, no noise.

let context: AudioContext | null = null;

function audioContext(): AudioContext | null {
  const Ctx = window.AudioContext;
  if (!Ctx) return null;
  context ??= new Ctx({ latencyHint: "interactive" });
  return context;
}

function startTick(ctx: AudioContext): void {
  const now = ctx.currentTime;
  const osc = ctx.createOscillator();
  const gain = ctx.createGain();
  osc.type = "sine";
  osc.frequency.setValueAtTime(988, now);
  osc.frequency.exponentialRampToValueAtTime(494, now + 0.04);
  gain.gain.setValueAtTime(0.0001, now);
  gain.gain.exponentialRampToValueAtTime(0.045, now + 0.006);
  gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.05);
  osc.connect(gain);
  gain.connect(ctx.destination);
  osc.start(now);
  osc.stop(now + 0.055);
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
