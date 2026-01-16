import { callApi } from "./index";

export interface FactResponse {
  text: string;
}

export async function getRandomFact(): Promise<FactResponse> {
  const response = await callApi({
    path: "/api/facts/random",
    method: "GET",
  });

  if (response.status >= 200 && response.status < 300) {
    return response.data as FactResponse;
  } else {
    throw new Error(`Failed to fetch fact: Status ${response.status}`);
  }
}
