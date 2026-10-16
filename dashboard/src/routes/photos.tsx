import { createFileRoute, useRouter } from "@tanstack/react-router"
import { PageHead } from "@/components/page-head"
import { PhotoGallery } from "@/components/photo-gallery"
import { getGallery } from "@/lib/photos.functions"
import { photoSearch, unavailablePhotos } from "@/lib/photos"

export const Route = createFileRoute("/photos")({
  validateSearch: photoSearch,
  loaderDeps: ({ search }) => ({ album: search.album, page: search.page }),
  loader: ({ deps }) =>
    getGallery({ data: deps }).catch(() => ({
      albums: unavailablePhotos,
      page: null,
      selectedAlbum: null,
    })),
  staleTime: 0,
  gcTime: 0,
  headers: () => ({ "Cache-Control": "no-store" }),
  head: () => ({ meta: [{ title: "Photos · OpenHome" }] }),
  component: PhotosPage,
  pendingComponent: () => (
    <p role="status" className="dir-pending">
      Loading photos…
    </p>
  ),
})

function PhotosPage(): React.JSX.Element {
  const data = Route.useLoaderData()
  const navigate = Route.useNavigate()
  const router = useRouter()

  return (
    <section>
      <PageHead title="Photos" />
      <PhotoGallery
        {...data}
        select={(album, page) =>
          void navigate({ search: { album, page }, resetScroll: true })
        }
        refresh={() => router.invalidate({ sync: true })}
      />
    </section>
  )
}
