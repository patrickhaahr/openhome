import { splitProps, type JSX } from "solid-js";
import { cn } from "@/lib/utils";

interface InputProps extends JSX.InputHTMLAttributes<HTMLInputElement> {}

const Input = (props: InputProps) => {
  const [local, others] = splitProps(props, ["class", "type"]);
  return (
    <input
      type={local.type}
      class={cn(
        "flex h-10 w-full rounded-md border border-border bg-bg-tertiary px-3 py-2 text-sm ring-offset-background file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-text-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50",
        local.class
      )}
      {...others}
    />
  );
};

export { Input };
