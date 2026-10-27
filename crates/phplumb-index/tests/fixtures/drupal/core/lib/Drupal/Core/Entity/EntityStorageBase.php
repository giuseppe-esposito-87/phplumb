<?php
namespace Drupal\Core\Entity;

use Drupal\Core\Extension\ModuleHandlerInterface;

abstract class EntityStorageBase
{
    protected $entityTypeId;

    /**
     * @return \Drupal\Core\Extension\ModuleHandlerInterface
     */
    protected function moduleHandler()
    {
    }

    protected function invokeHook($hook, EntityInterface $entity)
    {
        $this->moduleHandler()->/*@target:presave*//*@target:entity_presave*/invokeAll($this->entityTypeId . '_' . $hook, [$entity]);
        $this->moduleHandler()->/*@target:presave*//*@target:entity_presave*/invokeAll('entity_' . $hook, [$entity]);
    }
}
