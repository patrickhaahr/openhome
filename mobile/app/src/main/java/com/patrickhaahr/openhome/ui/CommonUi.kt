package com.patrickhaahr.openhome.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.error
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphIcon
import com.patrickhaahr.openhome.ui.design.GlyphKey
import com.patrickhaahr.openhome.ui.design.KeyTone
import com.patrickhaahr.openhome.ui.design.key
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.theme.Type

val ScreenMargin: Dp = 16.dp
val SlabShape = RoundedCornerShape(28.dp)
val FieldShape = RoundedCornerShape(20.dp)

/**
 * Words the screen says with symbols instead still belong to anyone listening: removed from sight,
 * never from the app. A real pixel, because Android leaves out what has no size.
 */
@Composable
fun Spoken(text: String, modifier: Modifier = Modifier, isHeading: Boolean = false, live: Boolean = false) {
    Box(
        modifier
            .size(1.dp)
            .semantics {
                contentDescription = text
                if (isHeading) heading()
                if (live) liveRegion = LiveRegionMode.Polite
            },
    )
}

/**
 * [content] with lines for assistive technology laid over its corner, so they take no room and no gap
 * in the layout. They are one pixel wide and sit where nothing is ever pressed.
 */
@Composable
fun WithSpoken(
    modifier: Modifier = Modifier,
    spoken: @Composable ColumnScope.() -> Unit,
    content: @Composable () -> Unit,
) {
    Box(modifier) {
        content()
        Column(Modifier.width(1.dp), content = spoken)
    }
}

/** One line of what went wrong, with the whole of it a tap away: several at once must not bury the screen. */
@Composable
fun ProblemTag(message: String, modifier: Modifier = Modifier) {
    var open by rememberSaveable(message) { mutableStateOf(false) }
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 44.dp)
            .key(if (open) "Hide details" else "Show details", { open = !open }, shape = FieldShape, depth = 0.99f)
            .padding(horizontal = 6.dp, vertical = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = if (open) Alignment.Top else Alignment.CenterVertically,
    ) {
        GlyphIcon(Glyph.Warn, tint = Lit.Alarm, size = 18.dp)
        Text(
            text = message,
            style = Type.Body,
            color = Lit.Alarm,
            maxLines = if (open) Int.MAX_VALUE else 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f).semantics { error(message) },
        )
        Spoken(message)
    }
}

/**
 * A destination's head: a symbol instead of a title, the figure that belongs to it, and what can be
 * done here at its far side. Back is deliberately absent: system back and the gesture provide it.
 */
@Composable
fun ScreenHeader(
    title: String,
    glyph: Glyph? = null,
    subtitle: String? = null,
    onOpenProfile: (() -> Unit)? = null,
    figure: String? = null,
    actions: @Composable RowScope.() -> Unit = {},
) {
    WithSpoken(
        spoken = {
            Spoken(title, isHeading = true)
            subtitle?.let { Spoken(it) }
        },
    ) {
        Row(
            modifier = Modifier.fillMaxWidth().heightIn(min = 72.dp).padding(start = ScreenMargin + 4.dp, end = ScreenMargin),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            if (glyph != null) GlyphIcon(glyph, tint = Lit.Lamp, size = 30.dp)
            if (figure != null) Text(figure, style = Type.Title, color = Lit.Ink)
            Spacer(Modifier.weight(1f))
            actions()
            if (onOpenProfile != null) ProfileKey(onOpenProfile)
        }
    }
}

@Composable
fun ProfileKey(onClick: () -> Unit) {
    GlyphKey(Glyph.Person, "Open Profile", onClick, tone = KeyTone.Raised, size = 48.dp)
}

/** One raised block of the screen; a tone step away from the black, with no line and no shadow. */
@Composable
fun Slab(
    modifier: Modifier = Modifier,
    color: Color = Lit.Slab,
    padding: Dp = 16.dp,
    arrangement: Arrangement.Vertical = Arrangement.spacedBy(12.dp),
    content: @Composable ColumnScope.() -> Unit,
) {
    Column(
        modifier.background(color, SlabShape).padding(padding),
        verticalArrangement = arrangement,
        content = content,
    )
}

/** A large symbol in the empty place. The words go on only when they tell the user what to do. */
@Composable
fun EmptyState(
    title: String,
    message: String,
    glyph: Glyph = Glyph.Barbell,
    readable: Boolean = false,
    action: @Composable () -> Unit = {},
) {
    Column(
        modifier = Modifier.fillMaxSize().padding(32.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        GlyphIcon(glyph, tint = if (readable) Lit.Alarm else Lit.Faint, size = 96.dp)
        Spoken(title)
        if (readable) {
            Text(
                text = message,
                style = Type.Body,
                color = Lit.Muted,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(top = 16.dp),
            )
        } else {
            Spoken(message)
        }
        Box(Modifier.padding(top = 16.dp)) { action() }
    }
}

/** A value, its symbol and its unit. [label] is what it is called for anyone who can't see the symbol. */
@Composable
fun FormField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    modifier: Modifier = Modifier,
    glyph: Glyph? = null,
    unit: String? = null,
    hint: String? = null,
    problem: String? = null,
    enabled: Boolean = true,
    singleLine: Boolean = true,
    capitalization: KeyboardCapitalization = KeyboardCapitalization.Sentences,
    keyboardType: KeyboardType = KeyboardType.Text,
    imeAction: ImeAction = ImeAction.Next,
    autoCorrect: Boolean = true,
    onDone: (() -> Unit)? = null,
    visualTransformation: VisualTransformation = VisualTransformation.None,
    well: Color = Lit.Slab,
    trailing: @Composable () -> Unit = {},
) {
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            singleLine = singleLine,
            enabled = enabled,
            textStyle = Type.Figure.copy(color = if (enabled) Lit.Ink else Lit.Muted),
            cursorBrush = SolidColor(Lit.Lamp),
            visualTransformation = visualTransformation,
            keyboardOptions =
                KeyboardOptions(
                    capitalization = capitalization,
                    autoCorrectEnabled = autoCorrect,
                    keyboardType = keyboardType,
                    imeAction = imeAction,
                ),
            keyboardActions = KeyboardActions(onDone = onDone?.let { done -> { done() } }),
            modifier = Modifier.fillMaxWidth(),
            decorationBox = { inner ->
                Row(
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .heightIn(min = 72.dp)
                            .background(if (problem != null) Lit.AlarmDeep else well, FieldShape)
                            .padding(start = 20.dp, end = if (unit != null) 20.dp else 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    if (glyph != null) {
                        GlyphIcon(glyph, tint = if (problem != null) Lit.Alarm else Lit.Lamp, size = 26.dp)
                        Spacer(Modifier.width(16.dp))
                    }
                    Box(
                        Modifier
                            .weight(1f)
                            .padding(vertical = 14.dp)
                            .semantics { contentDescription = label },
                        contentAlignment = Alignment.CenterStart,
                    ) {
                        if (value.isEmpty() && hint != null) {
                            Text(hint, style = Type.Body, color = Lit.Faint, maxLines = 1)
                        }
                        inner()
                    }
                    if (unit != null) Text(unit, style = Type.Name, color = Lit.Muted)
                    trailing()
                }
            },
        )
        problem?.let { ProblemText(it) }
    }
}

/** Failure stays visible and in words: the one thing a symbol can't say well enough. */
@Composable
fun ProblemText(message: String, modifier: Modifier = Modifier) {
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        GlyphIcon(Glyph.Warn, tint = Lit.Alarm, size = 16.dp, modifier = Modifier.padding(top = 2.dp))
        Text(
            text = message,
            style = Type.Body,
            color = Lit.Alarm,
            modifier = Modifier.semantics { error(message) },
        )
    }
}
