import { splitProps, type Component, type ComponentProps } from "solid-js";
import { cn } from "@/lib/utils";

const ScrollArea: Component<ComponentProps<"div">> = (props) => {
  const [local, others] = splitProps(props, ["class", "children"]);
  return (
    <div
      data-slot="scroll-area"
      class={cn(
        "relative overflow-hidden",
        local.class
      )}
      {...others}
    >
      <div
        data-slot="scroll-area-viewport"
        class="h-full w-full overflow-y-auto"
      >
        {local.children}
      </div>
    </div>
  );
};

const ScrollBar: Component<ComponentProps<"div">> = (props) => {
  return <div {...props} />;
};

export { ScrollArea, ScrollBar };
