import { PNG } from "pngjs"
import { describe, expect, it } from "vitest"
import { compare } from "./compare.ts"

function flat(width: number, height: number, grey: number): PNG {
  const png = new PNG({ width, height })

  for (let index = 0; index < png.data.length; index += 4)
    png.data.set([grey, grey, grey, 255], index)

  return png
}

const encode = (png: PNG) => PNG.sync.write(png)

describe("parity compare", () => {
  it("reads byte-equal screenshots as unchanged without a diff", () => {
    const shot = encode(flat(4, 3, 200))
    const result = compare(shot, Buffer.from(shot))

    expect(result).toMatchObject({ pixels: 0, total: 12, sizeChanged: false })
    expect(result.diff).toBeNull()
  })

  it("counts a single channel step of one pixel", () => {
    const changed = flat(4, 3, 200)
    changed.data[(1 * 4 + 2) * 4] = 201
    const result = compare(encode(flat(4, 3, 200)), encode(changed))

    expect(result.pixels).toBe(1)
    expect(result.diff).not.toBeNull()
  })

  it("counts the area only one side covers when the size changes", () => {
    const result = compare(encode(flat(4, 3, 200)), encode(flat(4, 5, 200)))

    expect(result).toMatchObject({ pixels: 8, total: 20, sizeChanged: true })
  })

  it("draws changed pixels red in the diff", () => {
    const changed = flat(2, 1, 200)
    changed.data.set([0, 0, 0, 255], 4)
    const result = compare(encode(flat(2, 1, 200)), encode(changed))
    const diff = PNG.sync.read(result.diff ?? Buffer.alloc(0))

    expect([...diff.data.subarray(4, 8)]).toEqual([255, 0, 0, 255])
    expect(diff.data[0]).not.toBe(255)
  })
})
