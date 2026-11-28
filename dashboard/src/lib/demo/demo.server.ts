import "@tanstack/react-start/server-only"
import { getCookie } from "@tanstack/react-start/server"
import { trainingDayOn } from "../workout"
import { dataModeCookieName, parseDataMode, type DataMode } from "./data-mode"
import type { Json } from "./date-shift"
import {
  answerFromSnapshot,
  isDemoRoute,
  snapshotFile,
  type DemoRequest,
  type Snapshot,
} from "./snapshot"

let loaded: Promise<Snapshot> | undefined

function loadSnapshot(): Promise<Snapshot> {
  loaded ??= import("./snapshot.json").then((module) =>
    snapshotFile.parse(module.default)
  )

  return loaded
}

export function dataModeOfRequest(): DataMode {
  try {
    return parseDataMode(getCookie(dataModeCookieName))
  } catch {
    return "live"
  }
}

function reply(status: number, body: Json): Response {
  return Response.json(body, { status })
}

export async function answerDemo(
  request: DemoRequest & { readonly method: string }
): Promise<Response | undefined> {
  if (!isDemoRoute(request.path) || dataModeOfRequest() !== "demo")
    return undefined

  if (request.method !== "GET")
    return reply(405, {
      error: "Demo data is read-only. Switch to live data to save changes.",
    })

  const answer = answerFromSnapshot(
    await loadSnapshot(),
    request,
    trainingDayOn(new Date())
  )

  return answer.state === "found"
    ? reply(200, answer.body)
    : reply(404, { error: "Demo data has nothing for this request." })
}
