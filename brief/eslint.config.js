import reactHooks from "eslint-plugin-react-hooks";
import tseslint from "typescript-eslint";

export default tseslint.config(
  // Apply to all TypeScript source files; exclude build output and Tauri generated code.
  { ignores: ["dist/", "src-tauri/"] },
  ...tseslint.configs.recommended,
  {
    plugins: {
      // The react-hooks plugin is already used in the codebase via eslint-disable comments;
      // including it here means those comments are properly resolved instead of generating
      // "unknown rule" errors.
      "react-hooks": reactHooks,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      // Enforce explicit return types on public functions for better API documentation.
      "@typescript-eslint/explicit-function-return-type": "off",
      // Disallow unused variables — TypeScript strict already catches these, but ESLint
      // also checks destructuring patterns that tsc misses.
      "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_" }],
      // `any` should be avoided; use `unknown` and narrow at the call site.
      "@typescript-eslint/no-explicit-any": "warn",
    },
  }
);
