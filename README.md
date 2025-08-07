# pvbc

pvbc, or "Page View Badge Counter" is a little API which gives a shields.io like
badge with a page view count!

## Demo

Demo is available [here](https://pvbc.e.hackclub.app), includes instructions.

### For SoM

You can use this API on your SoM profile with custom CSS! See this example.

> [!IMPORTANT]
> You must get a UUID from the demo link before you can use this.

The following places a badge under your username. Replace the URL with your URL
along with query parameters.

```css
h1.text-2xl > a:nth-child(1) > span:nth-child(1)::after {
  content: "";
  display: inline-block;
  width: auto;
  height: 2rem;
  background: url("the link you got from the api") no-repeat center center;
  background-size: contain;
  vertical-align: middle;
}
```

## DIY

Set these in your `.env`:

```sh
POSTGRESQL_CONNECTION_URL= # postgres:// etc
HOST=IP:PORT # 0.0.0.0:3000
```

Run with `cargo`

```sh
cargo r -r
```
