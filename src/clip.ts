/** Longer clips add little to a cloned voice and slow down every piece read in it. */
export const MAX_SECS = 20;
const RATE = 24_000;

/** A recording or audio file in any format the browser reads, as the 16-bit little-endian mono samples at 24 kHz
 * that yap clones from, cut to `MAX_SECS`. */
export async function toClip(blob: Blob): Promise<Uint8Array> {
  // Decoding in a 24 kHz context resamples too.
  const audio = await new OfflineAudioContext(1, 1, RATE).decodeAudioData(await blob.arrayBuffer());
  const channels = Array.from({ length: audio.numberOfChannels }, (_, index) =>
    audio.getChannelData(index),
  );
  const length = Math.min(audio.length, RATE * MAX_SECS);
  const pcm = new DataView(new ArrayBuffer(length * 2));
  for (let index = 0; index < length; index++) {
    const mixed = channels.reduce((sum, channel) => sum + channel[index], 0) / channels.length;
    pcm.setInt16(index * 2, Math.max(-1, Math.min(1, mixed)) * 32767, true);
  }
  return new Uint8Array(pcm.buffer);
}
