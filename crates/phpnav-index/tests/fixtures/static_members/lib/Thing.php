<?php
namespace Lib;

class /*@target:thing*/Thing extends Base
{
    use Makes;

    const /*@target:const*/VERSION = 2;
    public static /*@target:sprop*/$count = 0;

    public function __construct()
    {
        /*@cursor:base*/parent::/*@cursor:parentctor*/__construct();
        echo /*@cursor:thing*/self::/*@cursor:const*/VERSION;
        echo /*@cursor:thing*/static::/*@cursor:const*/VERSION;
        static::/*@cursor:tmake*/fromTrait();
        echo self::/*@cursor:iconst*/LIMIT;
    }

    public static function /*@target:make*/make(): /*@cursor:thing*/static
    {
        return new /*@cursor:thing*/self();
    }
}
