import { JSX } from "react";
import { useRouter, Link } from "tuono";

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

      <div></div>

      <Link href="/">Home Page</Link>

      <div></div>

      <button
        onClick={() => {
          router.push("/");
        }}
      >
        About
      </button>
    </>
  );
}
