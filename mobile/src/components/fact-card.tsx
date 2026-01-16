import { Component, createResource, Show, Suspense } from "solid-js";
import { getRandomFact } from "../api";
import { Button } from "./ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "./ui/card";
import { Skeleton } from "./ui/skeleton";

const FactCard: Component = () => {
  const [fact, { refetch }] = createResource(getRandomFact);

  return (
    <Card class="w-full">
      <CardHeader>
        <CardTitle>Random Fact</CardTitle>
        <CardDescription>Useless knowledge for your day</CardDescription>
      </CardHeader>
      <CardContent>
        <Suspense
          fallback={
            <div class="space-y-2">
              <Skeleton class="h-4 w-full" />
              <Skeleton class="h-4 w-[90%]" />
              <Skeleton class="h-4 w-[80%]" />
            </div>
          }
        >
          <Show
            when={!fact.error}
            fallback={
              <div class="rounded-md bg-destructive/10 p-4 text-sm text-destructive">
                Failed to load fact: {fact.error?.message}
              </div>
            }
          >
            <p class="leading-7">{fact()?.text}</p>
          </Show>
        </Suspense>
      </CardContent>
      <CardFooter>
        <Button 
          onClick={() => refetch()} 
          disabled={fact.loading}
          class="w-full sm:w-auto"
        >
          {fact.loading ? "Loading..." : "Refresh Fact"}
        </Button>
      </CardFooter>
    </Card>
  );
};

export default FactCard;
