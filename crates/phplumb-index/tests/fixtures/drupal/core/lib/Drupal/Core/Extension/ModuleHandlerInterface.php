<?php
namespace Drupal\Core\Extension;

interface ModuleHandlerInterface
{
    public function invoke($module, $hook, array $args = []);

    public function invokeAll($hook, array $args = []);

    public function invokeAllWith(string $hook, callable $callback): void;

    public function alter($type, &$data, &$context1 = NULL, &$context2 = NULL);
}
