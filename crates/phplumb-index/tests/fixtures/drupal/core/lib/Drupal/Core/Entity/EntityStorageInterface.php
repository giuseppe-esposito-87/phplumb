<?php
namespace Drupal\Core\Entity;

interface EntityStorageInterface
{
    /** @return \Drupal\Core\Entity\EntityInterface|null */
    public function load($id);

    /** @return \Drupal\Core\Entity\EntityInterface[] */
    public function loadMultiple(?array $ids = null);

    /** @return \Drupal\Core\Entity\EntityInterface */
    public function create(array $values = []);
}

interface EntityInterface
{
    public function id();
}

interface EntityTypeManagerInterface
{
    /** @return \Drupal\Core\Entity\EntityStorageInterface */
    public function getStorage($entity_type_id);
}

abstract class EntityStorageBase implements EntityStorageInterface
{
    public function load($id)
    {
    }

    public function loadMultiple(?array $ids = null)
    {
    }

    public function create(array $values = [])
    {
    }
}

abstract class ContentEntityBase implements EntityInterface
{
    public function id()
    {
    }
}
