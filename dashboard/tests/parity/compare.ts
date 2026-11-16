import { PNG } from "pngjs"

type Size = { readonly width: number; readonly height: number }

export type Comparison = {
  /** Pixels whose RGBA differs at all; there is no tolerance, so the noise floor is 0. */
  readonly pixels: number
  readonly total: number
  readonly sizeChanged: boolean
  readonly before: Size
  readonly after: Size
  /** Changed pixels in red over a faded copy of the before image; null when nothing changed. */
  readonly diff: Buffer | null
}

/** Opaque magenta, so area that only one image covers always counts as changed. */
const padding = [255, 0, 255, 255]

function size(png: Buffer): Size {
  return { width: png.readUInt32BE(16), height: png.readUInt32BE(20) }
}

function padded(image: PNG, width: number, height: number): Uint8Array {
  const out = new Uint8Array(width * height * 4)

  for (let y = 0; y < height; y += 1)
    for (let x = 0; x < width; x += 1) {
      const to = (y * width + x) * 4
      const from = (y * image.width + x) * 4
      const inside = x < image.width && y < image.height

      for (let channel = 0; channel < 4; channel += 1)
        out[to + channel] = inside
          ? (image.data[from + channel] ?? 0)
          : (padding[channel] ?? 0)
    }

  return out
}

/** Counts every differing pixel between two PNG screenshots, padding mismatched sizes. */
export function compare(beforePng: Buffer, afterPng: Buffer): Comparison {
  const before = size(beforePng)
  const after = size(afterPng)

  if (beforePng.equals(afterPng))
    return {
      pixels: 0,
      total: before.width * before.height,
      sizeChanged: false,
      before,
      after,
      diff: null,
    }

  const width = Math.max(before.width, after.width)
  const height = Math.max(before.height, after.height)
  const a = padded(PNG.sync.read(beforePng), width, height)
  const b = padded(PNG.sync.read(afterPng), width, height)
  const out = new PNG({ width, height })
  let pixels = 0

  for (let index = 0; index < width * height * 4; index += 4) {
    const same =
      a[index] === b[index] &&
      a[index + 1] === b[index + 1] &&
      a[index + 2] === b[index + 2] &&
      a[index + 3] === b[index + 3]

    if (same) {
      const luma =
        0.299 * (a[index] ?? 0) +
        0.587 * (a[index + 1] ?? 0) +
        0.114 * (a[index + 2] ?? 0)

      const faded = 255 - 0.1 * (255 - luma)
      out.data.set([faded, faded, faded, 255], index)
    } else {
      pixels += 1
      out.data.set([255, 0, 0, 255], index)
    }
  }

  const sizeChanged =
    before.width !== after.width || before.height !== after.height

  return {
    pixels,
    total: width * height,
    sizeChanged,
    before,
    after,
    diff: pixels === 0 ? null : PNG.sync.write(out),
  }
}
