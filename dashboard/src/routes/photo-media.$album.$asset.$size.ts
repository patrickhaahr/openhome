import { createFileRoute } from "@tanstack/react-router"

export const Route = createFileRoute("/photo-media/$album/$asset/$size")({
  server: {
    handlers: {
      GET: async ({ params }) => {
        const { readPhotoMedia } = await import("@/lib/photos.server")

        return readPhotoMedia(params)
      },
    },
  },
})
