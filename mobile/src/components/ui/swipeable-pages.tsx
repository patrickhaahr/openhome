import type { Accessor, JSX } from "solid-js";
import { createSignal, onCleanup, onMount, createEffect, For } from "solid-js";

interface SwipeablePagesProps {
  currentIndex: Accessor<number>;
  onIndexChange: (index: number) => void;
  onScrollDirectionChange?: (direction: "up" | "down") => void;
  children: JSX.Element[];
}

const SWIPE_THRESHOLD = 50;
const VELOCITY_THRESHOLD = 0.3;

const SCROLL_THRESHOLD = 10;

const SwipeablePages = (props: SwipeablePagesProps) => {
  let containerRef: HTMLDivElement | undefined;
  const pageRefs: HTMLDivElement[] = [];
  
  const [touchStart, setTouchStart] = createSignal<{ x: number; y: number; time: number } | null>(null);
  const [touchDelta, setTouchDelta] = createSignal(0);
  const [isDragging, setIsDragging] = createSignal(false);
  const [isHorizontalSwipe, setIsHorizontalSwipe] = createSignal<boolean | null>(null);
  
  // Scroll tracking - per page to avoid cross-page interference
  const scrollState = new Map<HTMLDivElement, { lastScrollY: number; accumulator: number }>();
  
  const pageCount = () => props.children.length;

  // Reset delta when index changes externally (e.g., from bottom nav)
  createEffect(() => {
    props.currentIndex();
    setTouchDelta(0);
  });

  const handleTouchStart = (e: TouchEvent) => {
    const touch = e.touches[0];
    setTouchStart({ x: touch.clientX, y: touch.clientY, time: Date.now() });
    setIsHorizontalSwipe(null);
    setIsDragging(true);
  };

  const handleTouchMove = (e: TouchEvent) => {
    const start = touchStart();
    if (!start) return;

    const touch = e.touches[0];
    const deltaX = touch.clientX - start.x;
    const deltaY = touch.clientY - start.y;

    // Determine swipe direction on first significant movement
    if (isHorizontalSwipe() === null && (Math.abs(deltaX) > 10 || Math.abs(deltaY) > 10)) {
      setIsHorizontalSwipe(Math.abs(deltaX) > Math.abs(deltaY));
    }

    // Only track horizontal swipes
    if (isHorizontalSwipe() === true) {
      e.preventDefault();
      
      // Add resistance at edges
      const currentIdx = props.currentIndex();
      const isAtStart = currentIdx === 0 && deltaX > 0;
      const isAtEnd = currentIdx === pageCount() - 1 && deltaX < 0;
      
      if (isAtStart || isAtEnd) {
        // Rubber band effect at edges
        setTouchDelta(deltaX * 0.3);
      } else {
        setTouchDelta(deltaX);
      }
    }
  };

  const handleTouchEnd = () => {
    const start = touchStart();
    const delta = touchDelta();
    
    if (start && isHorizontalSwipe() === true) {
      const elapsed = Date.now() - start.time;
      const velocity = Math.abs(delta) / elapsed;
      
      const currentIdx = props.currentIndex();
      const shouldNavigate = Math.abs(delta) > SWIPE_THRESHOLD || velocity > VELOCITY_THRESHOLD;
      
      if (shouldNavigate) {
        if (delta > 0 && currentIdx > 0) {
          // Swipe right -> go to previous page
          props.onIndexChange(currentIdx - 1);
        } else if (delta < 0 && currentIdx < pageCount() - 1) {
          // Swipe left -> go to next page
          props.onIndexChange(currentIdx + 1);
        }
      }
    }
    
    setTouchStart(null);
    setTouchDelta(0);
    setIsDragging(false);
    setIsHorizontalSwipe(null);
  };

  const handleScroll = (e: Event) => {
    const target = e.target as HTMLDivElement;
    const currentScrollY = target.scrollTop;
    
    // Get or initialize scroll state for this specific page
    let state = scrollState.get(target);
    if (!state) {
      state = { lastScrollY: currentScrollY, accumulator: 0 };
      scrollState.set(target, state);
      return; // Skip first scroll event to establish baseline
    }
    
    const delta = currentScrollY - state.lastScrollY;
    
    // Accumulate scroll to avoid jitter
    if (Math.sign(delta) !== Math.sign(state.accumulator)) {
      state.accumulator = 0;
    }
    state.accumulator += delta;
    
    if (Math.abs(state.accumulator) > SCROLL_THRESHOLD) {
      // At top of page, always show navbar
      if (currentScrollY <= 0) {
        props.onScrollDirectionChange?.("up");
      } else {
        props.onScrollDirectionChange?.(state.accumulator > 0 ? "down" : "up");
      }
      state.accumulator = 0;
    }
    
    state.lastScrollY = currentScrollY;
  };

  onMount(() => {
    if (!containerRef) return;
    
    containerRef.addEventListener("touchstart", handleTouchStart, { passive: true });
    containerRef.addEventListener("touchmove", handleTouchMove, { passive: false });
    containerRef.addEventListener("touchend", handleTouchEnd, { passive: true });
    
    // Add scroll listeners to each page
    pageRefs.forEach((ref) => {
      ref.addEventListener("scroll", handleScroll, { passive: true });
    });
    
    onCleanup(() => {
      containerRef?.removeEventListener("touchstart", handleTouchStart);
      containerRef?.removeEventListener("touchmove", handleTouchMove);
      containerRef?.removeEventListener("touchend", handleTouchEnd);
      pageRefs.forEach((ref) => {
        ref.removeEventListener("scroll", handleScroll);
      });
    });
  });

  const getTransformPx = () => {
    if (!containerRef) return 0;
    const baseOffset = -props.currentIndex() * containerRef.clientWidth;
    return baseOffset + touchDelta();
  };

  return (
    <div 
      ref={containerRef}
      class="absolute inset-0 overflow-hidden"
      style={{ "touch-action": "pan-y pinch-zoom" }}
    >
      <div
        class="flex absolute inset-y-0"
        style={{
          transform: `translateX(${getTransformPx()}px)`,
          transition: isDragging() ? "none" : "transform 300ms cubic-bezier(0.4, 0, 0.2, 1)",
          width: `${pageCount() * 100}%`,
        }}
      >
        <For each={props.children}>
          {(page, index) => (
            <div 
              ref={(el) => (pageRefs[index()] = el)}
              class="flex-shrink-0 overflow-y-auto"
              style={{ width: `${100 / pageCount()}%` }}
            >
              {page}
            </div>
          )}
        </For>
      </div>
    </div>
  );
};

export default SwipeablePages;
