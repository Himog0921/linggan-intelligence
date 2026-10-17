import { useContext, type ButtonHTMLAttributes } from "react";
import { TopicMutationContext } from "./topicMutationContext";
type Props = ButtonHTMLAttributes<HTMLButtonElement> & {
  "data-mutation"?: string | boolean;
};
/** Preserve authored disabled conditions while every explicit write is pending. */
export function TopicButton(props: Props) {
  const pending = useContext(TopicMutationContext);
  return (
    <button
      {...props}
      disabled={
        props.disabled || (props["data-mutation"] !== undefined && pending)
      }
    />
  );
}
