<?php
namespace Drupal\Core\Config\Entity;

class ConfigEntityStorage extends \Drupal\Core\Entity\EntityStorageBase
{
    public function /*@target:configPrefix*/configPrefix()
    {
    }

    protected function invokeHook($hook, \Drupal\Core\Entity\EntityInterface $entity)
    {
        $this->moduleHandler()->/*@target:setting_presave*/invokeAll($this->entityTypeId . '_' . $hook, [$entity]);
        $this->moduleHandler()->/*@target:entity_presave*/invokeAll('entity_' . $hook, [$entity]);
    }
}

abstract class ConfigEntityBase implements \Drupal\Core\Entity\EntityInterface
{
    public function id()
    {
    }
}
