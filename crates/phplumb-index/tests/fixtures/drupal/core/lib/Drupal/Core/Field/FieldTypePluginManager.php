<?php
namespace Drupal\Core\Field;

use Drupal\Core\Field\Attribute\FieldType;
use Drupal\Core\Plugin\DefaultPluginManager;

class FieldTypePluginManager extends DefaultPluginManager
{
    public function __construct(\Traversable $namespaces, $cache_backend, $module_handler)
    {
        parent::__construct('Plugin/Field/FieldType', $namespaces, $module_handler, FieldItemInterface::class, FieldType::class, 'Drupal\Core\Field\Annotation\FieldType');
    }
}
