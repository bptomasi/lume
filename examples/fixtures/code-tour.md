# A quiet place for code

Syntax colors follow each language. Indentation stays intact.

## Rust
```rust
// Keep ownership explicit.
fn greet(name: &str) -> String {
    format!("hello, {name}")
}
```

## Terraform / HCL
```terraform
resource "aws_s3_bucket" "archive" {
    bucket = "lume-reading-archive"
    tags   = { purpose = "reading" }
}
```

## TypeScript
```typescript
interface Article { title: string; read: boolean; }
const next = (articles: Article[]) =>
    articles.find(article => !article.read);
```

## Go
```go
func greeting(name string) string {
    return "hello, " + name
}
```
