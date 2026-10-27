<?php
namespace Drupal\Core\Theme;

interface ThemeManagerInterface
{
    public function invoke(string $theme_key, string $hook, array $args = []);

    public function invokeAllWith(string $hook, callable $callback): void;

    public function alter($type, &$data, &$context1 = NULL, &$context2 = NULL);
}
