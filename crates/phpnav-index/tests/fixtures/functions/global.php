<?php

function /*@target:global*/global_only() {}
function /*@target:shadowed_global*/shadowed() {}

function /*@target:unrelated*/bar() {}
function /*@target:unrelated*/aliased_target() {}
