<?php
namespace Lib;

interface Translatable
{
    /** @return $this */
    public function getTranslation(string $langcode);
}

interface EntityInterface extends Translatable
{
    /** @return \Lib\FieldList */
    public function get(string $name);
}

class FieldList
{
    public function /*@target:first*/first(): void
    {
    }
}

abstract class BaseEntity implements EntityInterface
{
    /** {@inheritdoc} */
    public function getTranslation($langcode)
    {
        return $this;
    }

    /** {@inheritdoc} */
    public function get($name)
    {
        return new FieldList();
    }

    /** {@inheritdoc} */
    public function untyped()
    {
    }
}

class Paragraph extends BaseEntity
{
    public function /*@target:para_only*/onlyParagraph(): void
    {
    }
}

class Holder
{
    /** @var FieldList */
    protected $list;
}

class SubHolder extends Holder
{
    protected $list;

    public function go(): void
    {
        $this->list->/*@cursor:first*/first();
    }
}
