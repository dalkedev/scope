# Turn traffic into a lead

Select a captured or imported session and open **Analysis**. Snurpo computes observations locally, without sending the capture to a model or requiring an account.

## Read the session summary

The view shows request count, average duration, error count and total traffic. The **Endpoint map** groups observed endpoints with their request counts, errors, average time and volume. Request counts can include saved in-flight exchanges; timing and error observations use completed exchanges.

An insight can link back to evidence with **Inspect a request**. Use that exchange to check the body, timing, provenance and applied rules before deciding what caused a problem.

Repeated calls can be intentional. Recorded duration includes breakpoint waiting and body buffering, so it is not pure server latency. Mocked or blocked responses must be distinguished from upstream behavior.

## Compare two sessions

1. Select the baseline session in the sidebar.
2. Open **Compare sessions**.
3. Choose another saved session under **Compare with**.
4. Select **Compare**.

The comparison reports added, removed and changed observed endpoints, with status, timing and JSON-field differences where available. Imported sessions can participate too. Desktop grouping uses method, hostname and a normalized path, replacing numeric or UUID segments with `:id`; query, scheme and port do not separate those groups.

Capture comparable user flows when possible. A different page, account state or sequence of actions can change the sample even when the application code is unchanged. This desktop comparison is distinct from the stricter operation matching and authored contracts in the HAR CLI.

## Explore an observed API

Choose **Export > Observed OpenAPI** to infer an OpenAPI 3.1.1 document from the selected session. The result describes observed endpoints and bounded schemas; it does not infer required fields, authentication, enums, constraints or unseen responses. It can include synthetic responses with separate evidence counts.

Observed schemas are not authored requirements. If CI must reject a missing required field or an unchanged HTTP 500, write an explicit assertion in the separate [HAR comparison workflow](har-compare-ci.md).

An MCP client can request the same local analyses after you [share the relevant sessions](mcp.md). Its interpretation is separate from Snurpo's recorded observations.
