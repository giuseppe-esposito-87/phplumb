<?php
namespace Drupal\Core\Entity;

class EntityTypeManager extends \Drupal\Core\Plugin\DefaultPluginManager implements EntityTypeManagerInterface
{
    public function __construct(\Traversable $namespaces, $module_handler)
    {
        parent::__construct('Entity', $namespaces, $module_handler, EntityInterface::class);
        $this->discovery = NULL;
    }

    public function /*@target:storage*/getStorage(string $entity_type)
    {
    }
}
