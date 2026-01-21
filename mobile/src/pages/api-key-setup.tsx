import type { Component } from "solid-js";
import { createSignal } from "solid-js";
import { saveApiKey } from "../stores/config";
import { getHealthStatus } from "../api/health";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";

interface ApiKeySetupProps {
  onValidated: () => void;
}

const ApiKeySetup: Component<ApiKeySetupProps> = (props) => {
  const [apiKey, setApiKey] = createSignal("");
  const [status, setStatus] = createSignal("");
  const [isLoading, setIsLoading] = createSignal(false);

  const handleSave = async () => {
    const key = apiKey();
    if (!key.trim()) {
      setStatus("API key required");
      return;
    }

    setIsLoading(true);
    setStatus("Saving...");

    try {
      await saveApiKey(key);
      setStatus("Validating...");
      await getHealthStatus();
      setStatus("Success!");
      setTimeout(props.onValidated, 500);
    } catch (e: unknown) {
      setStatus(`Failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div class="flex min-h-screen items-center justify-center bg-bg-primary px-4">
      <Card class="w-full max-w-md">
        <CardHeader>
          <CardTitle>Welcome</CardTitle>
          <CardDescription>Enter your API key to access Home App</CardDescription>
        </CardHeader>
        <CardContent class="space-y-4">
          <div class="space-y-2">
            <Input
              type="password"
              value={apiKey()}
              onInput={(e) => setApiKey(e.currentTarget.value)}
              placeholder="Enter API key"
              disabled={isLoading()}
              onKeyDown={(e) => e.key === "Enter" && handleSave()}
            />
          </div>
          <Button
            onClick={handleSave}
            class="w-full"
            disabled={isLoading()}
          >
            {isLoading() ? "Saving..." : "Save & Continue"}
          </Button>
          {status() && (
            <p class={`text-sm ${status().startsWith("Success") ? "text-success" : "text-error"}`}>
              {status()}
            </p>
          )}
        </CardContent>
      </Card>
    </div>
  );
};

export default ApiKeySetup;
