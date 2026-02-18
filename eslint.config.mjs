import eslint from "@eslint/js";
import tseslint from "typescript-eslint";
import solid from "eslint-plugin-solid";
import eslintConfigPrettier from "eslint-config-prettier";

export default tseslint.config(
  eslint.configs.recommended,
  ...tseslint.configs.recommended,
  {
    plugins: { solid },
  },
  eslintConfigPrettier,
  {
    ignores: ["dist/**", "node_modules/**", "src-tauri/**"],
  },
  {
    rules: {
      "@typescript-eslint/no-explicit-any": "warn",
    },
  }
);
