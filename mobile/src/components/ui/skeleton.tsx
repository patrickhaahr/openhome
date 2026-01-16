import { splitProps, type Component, type ComponentProps } from "solid-js";
import { cn } from "@/lib/utils";

const Skeleton: Component<ComponentProps<"div">> = (props) => {
  const [local, others] = splitProps(props, ["class"]);
  return (
    <div
      class={cn("animate-pulse rounded-md bg-muted", local.class)}
      {...others}
    />
  );
};

export { Skeleton };
