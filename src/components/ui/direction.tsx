"use client";

import { DirectionProvider as BaseDirectionProvider } from "@base-ui/react/direction-provider";

function DirectionProvider({ direction = "rtl", ...props }: BaseDirectionProvider.Props) {
  return <BaseDirectionProvider direction={direction} {...props} />;
}

export { DirectionProvider };
