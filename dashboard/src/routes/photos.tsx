import { createFileRoute, Link, useRouter } from "@tanstack/react-router"
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
    })),
  staleTime: 0,
  gcTime: 0,
  headers: () => ({ "Cache-Control": "no-store" }),
  head: () => ({ meta: [{ title: "Photos · OpenHome" }] }),
  component: PhotosPage,
  pendingComponent: () => <p role="status">Loading photos…</p>,
})

function PhotosPage(): React.JSX.Element {
  const data = Route.useLoaderData()
  const navigate = Route.useNavigate()
  const router = useRouter()

  return (
    <section className="focused-page photos-page">
      <header className="photos-heading">
        <div>
          <p className="page-eyebrow">OpenHome / Photos</p>
          <h1 id="page-heading" tabIndex={-1}>
            Photos
          </h1>
          <p className="feature-intro">
            Favourite places. Small moments. Photos worth coming back to.
          </p>
        </div>
        <Link to="/" className="back-link">
          Back to Home
        </Link>
      </header>
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
