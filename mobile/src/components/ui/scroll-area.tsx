import { splitProps, type Component, type ComponentProps } from "solid-js";
import { cn } from "@/lib/utils";

type ScrollAreaProps = ComponentProps<"div"> & {
  viewportRef?: (el: HTMLDivElement) => void;
};

const ScrollArea: Component<ScrollAreaProps> = (props) => {
  const [local, others] = splitProps(props, ["class", "children", "viewportRef"]);
  return (
    <div
      data-slot="scroll-area"
      class={cn("relative overflow-hidden", local.class)}
      {...others}
    >
      <div
        data-slot="scroll-area-viewport"
        class="h-full w-full overflow-y-auto"
        ref={(el) => local.viewportRef?.(el)}
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
