<?php
namespace Drupal\Core\Field;

use Drupal\Core\Field\Attribute\FieldWidget;
use Drupal\Core\Plugin\DefaultPluginManager;

class WidgetPluginManager extends DefaultPluginManager
{
    public function __construct(\Traversable $namespaces, $cache_backend, $module_handler)
    {
        parent::__construct('Plugin/Field/FieldWidget', $namespaces, $module_handler, WidgetInterface::class, FieldWidget::class, 'Drupal\Core\Field\Annotation\FieldWidget');
    }

    public function createInstance($plugin_id, array $configuration = [])
    {
    }
}
