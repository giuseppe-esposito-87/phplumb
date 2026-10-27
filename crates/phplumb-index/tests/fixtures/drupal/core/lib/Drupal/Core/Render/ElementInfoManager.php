<?php
namespace Drupal\Core\Render;

use Drupal\Core\Plugin\DefaultPluginManager;
use Drupal\Core\Render\Attribute\RenderElement;

class ElementInfoManager extends DefaultPluginManager
{
    public function __construct(\Traversable $namespaces, $cache_backend, $module_handler)
    {
        parent::__construct('Element', $namespaces, $module_handler, ElementInterface::class, RenderElement::class, 'Drupal\Core\Render\Annotation\RenderElement');
    }

    public function createInstance($plugin_id, array $configuration = [])
    {
    }
}
