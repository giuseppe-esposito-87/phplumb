<?php
namespace Drupal\Core\Plugin;

use Drupal\Component\Plugin\PluginManagerInterface;

class DefaultPluginManager implements PluginManagerInterface
{
    public function __construct($subdir, \Traversable $namespaces, $module_handler, $plugin_interface = NULL, ?string $plugin_definition_attribute_name = NULL, $plugin_definition_annotation_name = NULL, array $additional_annotation_namespaces = [])
    {
    }

    public function createInstance($plugin_id, array $configuration = [])
    {
    }

    public function getDefinition($plugin_id, $exception_on_invalid = TRUE)
    {
    }

    public function hasDefinition($plugin_id)
    {
    }
}
