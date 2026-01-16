import { Component, createResource, Show, Suspense } from "solid-js";
import { getHealthStatus, getHealthUrl } from "../../api/health";
import { baseUrl } from "../../stores/config";
import { Button } from "./button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "./card";
import { Skeleton } from "./skeleton";

interface HealthData {
  url: string;
  status: string;
}

const fetchHealthData = async (): Promise<HealthData | null> => {
  const url = baseUrl();
  if (!url) return null;

  const [response, displayUrl] = await Promise.all([
    getHealthStatus(),
    getHealthUrl(),
  ]);

  return { url: displayUrl, status: response.status };
};

const ApiStatus: Component = () => {
  const [healthData, { refetch }] = createResource(baseUrl, fetchHealthData);

  return (
    <Card class="w-full">
      <CardHeader>
        <CardTitle>API Health</CardTitle>
        <CardDescription>Connection status and health information</CardDescription>
      </CardHeader>
      <CardContent>
        <Suspense
          fallback={
            <div class="space-y-2">
              <Skeleton class="h-4 w-full" />
              <Skeleton class="h-4 w-[80%]" />
            </div>
          }
        >
          <Show
            when={!healthData.error}
            fallback={
              <div class="rounded-md bg-destructive/10 p-4 text-sm text-destructive">
                Connection Failed: {healthData.error?.message}
              </div>
            }
          >
            <Show
              when={healthData()}
              fallback={
                <p class="text-muted-foreground">No configuration set</p>
              }
            >
              <div class="space-y-2">
                <p class="text-sm text-muted-foreground">
                  Request URL:{" "}
                  <span class="font-medium text-foreground break-all">
                    {healthData()?.url}
                  </span>
                </p>
                <p class="text-sm text-muted-foreground">
                  Status:{" "}
                  <span class="font-medium text-emerald-500">
                    {healthData()?.status}
                  </span>
                </p>
              </div>
            </Show>
          </Show>
        </Suspense>
      </CardContent>
      <CardFooter>
        <Button
          onClick={() => refetch()}
          disabled={healthData.loading}
          class="w-full sm:w-auto"
        >
          {healthData.loading ? "Refreshing..." : "Refresh"}
        </Button>
      </CardFooter>
    </Card>
  );
};

export default ApiStatus;
