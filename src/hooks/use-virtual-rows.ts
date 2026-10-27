import { useLayoutEffect, useState } from "react";
import { flushSync } from "react-dom";
import {
  elementScroll,
  observeElementOffset,
  observeElementRect,
  Virtualizer,
  type VirtualItem,
} from "@tanstack/virtual-core";

type VirtualRowsLayout = {
  readonly items: readonly VirtualItem[];
  readonly totalSize: number;
};

const EMPTY_LAYOUT: VirtualRowsLayout = { items: [], totalSize: 0 };

const virtualizerOptions = { observeElementRect, observeElementOffset, scrollToFn: elementScroll };

export function useVirtualRows({
  count,
  scrollElement,
  rowHeight,
  overscan,
}: {
  count: number;
  scrollElement: HTMLElement | null;
  rowHeight: number;
  overscan: number;
}) {
  const [layout, setLayout] = useState(EMPTY_LAYOUT);
  const [virtualizer] = useState(
    () =>
      new Virtualizer<HTMLElement, Element>({
        ...virtualizerOptions,
        count: 0,
        getScrollElement: () => null,
        estimateSize: () => rowHeight,
      }),
  );

  useLayoutEffect(() => virtualizer._didMount(), [virtualizer]);

  useLayoutEffect(() => {
    const publish = (instance: Virtualizer<HTMLElement, Element>) => {
      const items = instance.getVirtualItems();
      const totalSize = instance.getTotalSize();
      setLayout((current) =>
        current.items === items && current.totalSize === totalSize ? current : { items, totalSize },
      );
    };
    virtualizer.setOptions({
      ...virtualizerOptions,
      count,
      getScrollElement: () => scrollElement,
      estimateSize: () => rowHeight,
      overscan,
      onChange: (instance, sync) => {
        if (sync) flushSync(() => publish(instance));
        else publish(instance);
      },
    });
    virtualizer._willUpdate();
    publish(virtualizer);
  }, [virtualizer, count, scrollElement, rowHeight, overscan]);

  return {
    items: layout.items,
    totalSize: layout.totalSize,
    scrollToIndex: virtualizer.scrollToIndex,
  };
}
