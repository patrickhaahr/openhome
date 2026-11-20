import { createFileRoute, useRouter } from "@tanstack/react-router"
import { PageHead } from "@/components/page-head"
import { PhotoGallery } from "@/components/photo-gallery"
import { getGallery, getWallPage } from "@/lib/photos.functions"
import { photoSearch, unavailablePhotos } from "@/lib/photos"

export const Route = createFileRoute("/photos")({
  validateSearch: photoSearch,
  loaderDeps: ({ search }) => ({ album: search.album }),
  loader: ({ deps }) =>
    getGallery({ data: deps }).catch(() => ({
      albums: unavailablePhotos,
      view: null,
      wall: null,
      onThisDay: null,
    })),
  staleTime: Number.POSITIVE_INFINITY,
  gcTime: 0,
  headers: () => ({ "Cache-Control": "no-store" }),
  head: () => ({ meta: [{ title: "Photos · OpenHome" }] }),
  component: PhotosPage,
  pendingComponent: () => (
    <p role="status" className="inline-flex text-muted-ink">
      Loading photos…
    </p>
  ),
})

function PhotosPage(): React.JSX.Element {
  const data = Route.useLoaderData()
  const search = Route.useSearch()
  const navigate = Route.useNavigate()
  const router = useRouter()

  return (
    <section>
      <PageHead title="Photos" />
      <PhotoGallery
        {...data}
        photo={search.photo}
        open={(album) =>
          void navigate({ search: { album }, resetScroll: true })
        }
        show={(photo) =>
          void navigate({
            search: (previous) => ({ ...previous, photo }),
            replace: true,
            resetScroll: false,
          })
        }
        more={(album, page) => getWallPage({ data: { album, page } })}
        retry={() => router.invalidate({ sync: true })}
      />
    </section>
  )
}
