import { Button } from "@/components/ui/button";

// Placeholder until the settings screens land (step 2).
export default function App() {
  return (
    <main className="flex h-screen flex-col items-center justify-center gap-4">
      <h1 className="text-xl font-semibold">nfcd Settings</h1>
      <p className="text-sm text-muted-foreground">Coming soon.</p>
      <Button variant="outline" onClick={() => window.close()}>
        Close
      </Button>
    </main>
  );
}
