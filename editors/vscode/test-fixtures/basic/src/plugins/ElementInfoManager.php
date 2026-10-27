<?php
namespace Drupal\Core\Render;

use Drupal\Core\Plugin\DefaultPluginManager;
use Drupal\Core\Render\Attribute\FormElement;

class ElementInfoManager extends DefaultPluginManager
{
    public function __construct(\Traversable $namespaces, $module_handler)
    {
        parent::__construct('Element', $namespaces, $module_handler, NULL, FormElement::class);
    }
}
