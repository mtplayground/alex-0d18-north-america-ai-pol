type SourceCategoryTagProps = {
  category: "policy" | "news";
};

export function SourceCategoryTag({ category }: SourceCategoryTagProps) {
  const isNews = category === "news";

  return (
    <small className={`source-category-tag ${isNews ? "is-news" : "is-policy"}`}>
      {isNews ? "AI news" : "Government policy"}
    </small>
  );
}
