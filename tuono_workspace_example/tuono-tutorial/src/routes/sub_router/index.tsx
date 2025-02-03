import { JSX } from "react";
import { useRouter, Link } from "tuono";

export default function IndexPage(): JSX.Element {
  const router = useRouter();

  return (
    <>
      <p>pathname: {router.pathname}</p>

      <div></div>

      <Link
        href="/"
        className="button-link" // Add a class for styling
        onClick={(e) => {
          e.preventDefault(); // Prevent default link behavior
          router.push("/"); // Use router.push to navigate
        }}
      >
        Home Page
      </Link>

      <div></div>

      <Link
        href="/"
        className="button-link" // Add a class for styling
        onClick={(e) => {
          e.preventDefault(); // Prevent default link behavior
          router.push("/"); // Use router.push to navigate
        }}
      >
        Another homepage
      </Link>
    </>
  );
}
