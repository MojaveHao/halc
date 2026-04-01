# HAL – Hardware Assembly Lisp

**HAL** is a hardware description language that combines the expressive power of Lisp macros with the simplicity of S‑expressions. Write compact, parameterized hardware – the compiler expands your code at compile time and generates Verilog.

- **Zero dependencies** – The Rust compiler (`halc`) has an empty `[dependencies]` section.
- **Lisp‑style macros** – Use `expil!` and `!`-suffixed operators to generate hardware structures during compilation.
- **Fast generation** – A 128×128 systolic array (16,384 PEs) expands in **0.48 seconds** and produces over **130k lines** of Verilog.
- **Pure Rust** – Hand‑written parser, macro expander, and code generator.

## Example

### Simple counter

```lisp
(module counter
  (ports
    (input clk)
    (input rst_n)
    (output reg count 8))
  (process (or (posedge clk) (negedge rst_n))
    (if (not rst_n)
      (nb-write (signal count) 8'b0)
      (nb-write (signal count) (+ (signal count) 1'b1)))))
```

Generates Verilog:

```verilog
module counter (
    clk, rst_n, count );
    input clk;
    input rst_n;
    output reg [7:0] count;
    always @(posedge clk or negedge rst_n) begin
        if (~(rst_n)) begin
            count <= 8'b0;
        end
        else begin
            count <= (count + 1'b1);
        end
    end
endmodule

```

### Parameterized systolic array

Define a macro to generate an `N×N` systolic array:

```lisp
(macro define-systolic-array (name N data-width sum-width)
  (expil!
    (module name
      (ports
        (foreach! (between! 0 N) i
          (input (add! (str! a_in_) i) data-width))
        (foreach! (between! 0 N) i
          (input (add! (str! b_in_) i) data-width))
        (foreach! (between! 0 N) i
          (output (add! (str! sum_out_) i) sum-width)))
      ;; ... internal wires and PE instances ...
      )))

(define-systolic-array my_systolic_128x128 128 8 16)
```

Run the compiler and generate Verilog in milliseconds.

## Getting Started

1. **Build the compiler** (Rust 1.70+ required):

   ```bash
   git clone https://github.com/MojaveHao/halc.git
   cd hal
   cargo build --release
   ```

2. **Run the compiler on an example**:

   ```bash
   ./target/release/halc examples/systolic_array.hal
   ```

## Language Overview

- **S‑expression syntax** – All constructs are prefix lists.
- **Built‑in hardware primitives** – `module`, `ports`, `process`, `assign`, `instance`, etc.
- **Macro system** – `(macro name (params) (expil! ...))`. Inside `expil!`, symbols ending with `!` are evaluated at compile time.
- **Compile‑time reflection** – `(module-ports!)`, `(foreach!)`, `(between!)`, `(add!)`, `(str!)`, `(if!)`, etc.
- **Type‑inferred bitwidth** – Literals like `8'b1010`, `16'd123`.

## Performance

- Zero dependencies → small binary, fast compilation.
- Generated Verilog scales linearly with input size.
- Tested on 256×256 arrays (65k PEs) – runs in under 2 seconds.

## License

AGPLv3 License. See [LICENSE](LICENSE).

---

**HAL – bridging 1958 and 2026, one macro at a time.**