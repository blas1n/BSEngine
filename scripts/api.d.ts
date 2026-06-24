/**
 * BSEngine TypeScript API
 * Available in scripts via Deno.core.ops.*
 */
declare namespace bsengine {
    /** Log a message to the engine console */
    function log(message: string): void;
    /** Get the engine version */
    function version(): string;
}
