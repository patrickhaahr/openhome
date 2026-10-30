import { createFileRoute } from "@tanstack/react-router"

export const Route = createFileRoute("/meal-photo/$photo/$size")({
  server: {
    handlers: {
      GET: async ({ params }) => {
        const { readMealPhoto } = await import("@/lib/nutrition.server")

        return readMealPhoto(params)
      },
    },
  },
})
