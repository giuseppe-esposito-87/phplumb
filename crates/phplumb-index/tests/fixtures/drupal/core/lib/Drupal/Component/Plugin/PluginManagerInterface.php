<?php
namespace Drupal\Component\Plugin;

interface PluginManagerInterface
{
    public function createInstance($plugin_id, array $configuration = []);

    public function getDefinition($plugin_id, $exception_on_invalid = TRUE);

    public function hasDefinition($plugin_id);
}
