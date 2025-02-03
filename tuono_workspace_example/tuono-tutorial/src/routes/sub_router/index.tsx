import { JSX } from "react";
import { useRouter } from "tuono";

export default function IndexPage(): JSX.Element {
  const router = useRouter();

  return (
    <>
      <p>pathname: {router.pathname}</p>
      <button
        onClick={() => {
          router.push("/");
        }}
      >
        My link
      </button>
    </>
  );
}
